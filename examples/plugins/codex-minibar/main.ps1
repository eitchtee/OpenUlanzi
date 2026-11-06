# Shows Codex Minibar's quotas. Minibar does the fetching and holds the logins: this only asks its
# local bridge, the one its Stream Deck plugin uses, and draws the answer.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

$pluginId = 'openulanzi.codex-minibar'
$pollSeconds = 3
$catalogSeconds = 60
$views = @{}    # what is on a deck now, by instance
$sent = @{}     # last drawing of each instance, so an unchanged one isn't sent again
$offsets = @{}  # first provider each page shows
$snapshot = $null
$catalog = $null
$options = ''

function Send($message) {
    [Console]::Out.WriteLine(($message | ConvertTo-Json -Compress -Depth 12))
}

function Get-Endpoint {
    try {
        $path = Join-Path $env:APPDATA 'Codex Minibar\Codex Minibar\config\streamdeck-bridge.json'
        $endpoint = Get-Content -Raw -Encoding UTF8 $path | ConvertFrom-Json
        if ($endpoint.protocol -eq 1 -and $endpoint.host -and $endpoint.port -and $endpoint.token) { return $endpoint }
    } catch {}
    $null
}

# One request per connection, answered by one line. Null means Minibar isn't there.
function Ask($op, $extra = @{}) {
    $endpoint = Get-Endpoint
    if (-not $endpoint) { return $null }
    $client = New-Object Net.Sockets.TcpClient
    try {
        if (-not $client.ConnectAsync($endpoint.host, $endpoint.port).Wait(1500)) { return $null }
        $client.ReceiveTimeout = 1500
        $stream = $client.GetStream()
        $bytes = [Text.Encoding]::UTF8.GetBytes((($extra + @{ op = $op; token = $endpoint.token }) | ConvertTo-Json -Compress) + "`n")
        $stream.Write($bytes, 0, $bytes.Length)
        $line = [IO.StreamReader]::new($stream, [Text.UTF8Encoding]::new($false)).ReadLine()
        $answer = $line | ConvertFrom-Json
        if ($answer.ok) { return $answer }
    } catch {} finally { $client.Close() }
    $null
}

function Start-Minibar {
    $endpoint = Get-Endpoint
    $program = if ($endpoint.executable) { $endpoint.executable } else { Join-Path $env:LOCALAPPDATA 'Codex Minibar\codex-minibar.exe' }
    if (Test-Path $program) { Start-Process $program }
}

function Escape($text) {
    "$text".Replace('&', '&amp;').Replace('<', '&lt;').Replace('>', '&gt;')
}

function Status-Color($percent) {
    if ($null -eq $percent) { '#8490a3' } elseif ($percent -le 15) { '#e64a48' } elseif ($percent -le 50) { '#f59e0b' } else { '#34bc84' }
}

function Brand-Color($provider) {
    $rgb = $provider.brand_rgb
    if ($rgb) { '#{0:x2}{1:x2}{2:x2}' -f [int]$rgb[0], [int]$rgb[1], [int]$rgb[2] } else { '#f4f6fb' }
}

# Accounts of one provider sit side by side, so only the part after the separator tells them apart.
function Short-Name($provider) {
    ($provider.name -split '\s[\u00b7\u2014-]\s')[-1]
}

# Windows has trouble with the nine decimals Minibar can send, and only minutes matter here.
function Minutes-Left($time) {
    if (-not $time) { return $null }
    try {
        $trimmed = "$time" -replace '(\.\d{1,7})\d*', '$1'
        [Math]::Floor(([DateTimeOffset]::Parse($trimmed) - [DateTimeOffset]::UtcNow).TotalMinutes)
    } catch { $null }
}

function Countdown($time) {
    $left = Minutes-Left $time
    if ($null -eq $left -or $left -le 0) { return '' }
    $days = [Math]::Floor($left / 1440)
    $hours = [Math]::Floor(($left % 1440) / 60)
    if ($days -gt 0) { "${days}d ${hours}h" } else { "${hours}h $([int]($left % 60))m" }
}

# The catalog's label is in one language everywhere; the snapshot's follows Minibar's settings.
function Metric-Label($provider, $metric) {
    $entry = $catalog.providers | Where-Object { $_.id -eq $provider.id } | Select-Object -First 1
    $known = $entry.metrics | Where-Object { $_.id -eq $metric.id } | Select-Object -First 1
    if ($known) { $known.label } else { $metric.label }
}

# "5h session" to "5h", "Cursor Models" to "Cursor": enough for a key.
function Metric-Name($provider, $metric) {
    ("$(Metric-Label $provider $metric)" -split '\s')[0]
}

function Rows($provider) {
    @($provider.metrics | Where-Object { $null -ne $_.window.remaining_percent })
}

function Svg($width, $body) {
    "<svg xmlns='http://www.w3.org/2000/svg' width='$width' height='196'>$body</svg>"
}

function Words($x, $y, $size, $color, $text, $anchor = 'middle') {
    "<text x='$x' y='$y' font-family='Bahnschrift' font-size='$size' font-weight='700' fill='$color' text-anchor='$anchor'>$(Escape $text)</text>"
}

function Tile($percent, $name, $nameColor, $caption) {
    $color = Status-Color $percent
    $length = 2 * [Math]::PI * 52
    $arc = if ($percent -gt 0) {
        "<circle cx='98' cy='100' r='52' fill='none' stroke='$color' stroke-width='13' stroke-linecap='round' stroke-dasharray='$($length * $percent / 100) $length' transform='rotate(-90 98 100)'/>"
    }
    $value = if ($null -eq $percent) { '?' } else { "$percent%" }
    Svg 196 ((Words 98 30 25 $nameColor $name) +
        "<circle cx='98' cy='100' r='52' fill='none' stroke='#232a38' stroke-width='13'/>" + $arc +
        (Words 98 113 36 '#f4f6fb' $value) + (Words 98 186 24 '#8b95a9' $caption))
}

function Caption($name, $reset) {
    if ($reset) { "$name $([char]0xb7) $reset" } else { $name }
}

function Run($action, $params = @{}) {
    @{ plugin = @{ id = $pluginId; action = $action; params = $params } }
}

function Quota-Key($provider, $metric) {
    $window = $metric.window
    @{
        svg = (Tile $window.remaining_percent (Short-Name $provider) (Brand-Color $provider) (Caption (Metric-Name $provider $metric) (Countdown $window.resets_at)))
        background = '#10141d'
        action = (Run 'popup' @{ provider = $provider.id })
        hold = (Run 'refresh')
    }
}

function Providers {
    @($snapshot.providers | Where-Object { (Rows $_).Count -gt 0 })
}

function Every-Quota {
    foreach ($provider in Providers) {
        foreach ($metric in Rows $provider) { [pscustomobject]@{ provider = $provider; metric = $metric } }
    }
}

function Wide-Tightest {
    $tightest = Every-Quota | Sort-Object { $_.metric.window.remaining_percent } | Select-Object -First 1
    $window = $tightest.metric.window
    $percent = $window.remaining_percent
    $color = Status-Color $percent
    $reset = Countdown $window.resets_at
    $name = "$(Short-Name $tightest.provider) $(Metric-Name $tightest.provider $tightest.metric)"
    @{
        svg = (Svg 458 ((Words 24 44 28 (Brand-Color $tightest.provider) $name 'start') +
            (Words 24 132 92 '#f4f6fb' "$percent%" 'start') + (Words 434 132 34 '#8b95a9' $reset 'end') +
            "<rect x='24' y='154' width='410' height='16' rx='8' fill='#232a38'/><rect x='24' y='154' width='$(410 * $percent / 100)' height='16' rx='8' fill='$color'/>"))
        background = '#10141d'
        action = (Run 'popup' @{ provider = $tightest.provider.id })
        hold = (Run 'refresh')
    }
}

function Soonest-Key {
    $soonest = Every-Quota | Where-Object { (Minutes-Left $_.metric.window.resets_at) -gt 0 } |
        Sort-Object { Minutes-Left $_.metric.window.resets_at } | Select-Object -First 1
    if (-not $soonest) { return $null }
    @{
        svg = (Svg 196 ((Words 98 40 25 '#8b95a9' 'Next reset') + (Words 98 106 44 '#f4f6fb' (Countdown $soonest.metric.window.resets_at)) +
            (Words 98 168 24 (Brand-Color $soonest.provider) "$(Short-Name $soonest.provider) $(Metric-Name $soonest.provider $soonest.metric)")))
        background = '#10141d'
    }
}

# Quotas rarely move, so this is what shows a refresh worked.
function Updated {
    $left = @(Providers | ForEach-Object { Minutes-Left $_.sampled_at } | Where-Object { $null -ne $_ })
    if ($left.Count -eq 0) { return 'Refresh' }
    $minutes = [Math]::Max(0, -($left | Measure-Object -Maximum).Maximum)
    if ($minutes -lt 1) { 'Updated now' } elseif ($minutes -lt 60) { "Updated ${minutes}m" } else { "Updated $([Math]::Floor($minutes / 60))h" }
}

function Draw-Page($instance) {
    $keys = @{}
    0..10 | ForEach-Object { $keys["$_"] = $null }
    $keys['11'] = @{ label = (Updated); icon = 'ph:arrow-clockwise-bold'; action = (Run 'refresh') }
    $keys['12'] = @{ label = 'Back'; icon = 'ph:arrow-left-bold'; action = 'back' }
    $all = @(Providers)
    if ($all.Count -eq 0) {
        $keys['wide'] = @{
            svg = (Svg 458 ((Words 229 88 34 '#f4f6fb' 'Codex Minibar is not running') + (Words 229 132 26 '#8b95a9' 'Press to start it')))
            background = '#10141d'
            action = (Run 'start')
        }
    } else {
        $offset = [int]$offsets[$instance]
        if ($offset -ge $all.Count) { $offset = 0 }
        for ($column = 0; $column -lt 5 -and $offset + $column -lt $all.Count; $column++) {
            $provider = $all[$offset + $column]
            $rows = @(Rows $provider)
            $keys["$column"] = Quota-Key $provider $rows[0]
            if ($rows.Count -gt 1) { $keys["$(5 + $column)"] = Quota-Key $provider $rows[1] }
        }
        $keys['10'] = if ($all.Count -gt 5) {
            @{ label = 'More'; icon = 'ph:caret-right-bold'; action = (Run 'more' @{ instance = $instance }) }
        } else { Soonest-Key }
        $keys['wide'] = Wide-Tightest
    }
    Draw $instance @{ keys = $keys }
}

function Draw-Button($instance) {
    $wanted = $views[$instance].params
    $provider = Providers | Where-Object { $_.id -eq $wanted.provider -or $_.name -eq $wanted.provider } | Select-Object -First 1
    if (-not $provider) {
        $button = if ($snapshot) {
            @{ svg = (Tile $null "$($wanted.provider)" '#8b95a9' 'No data'); background = '#10141d'; action = (Run 'popup'); hold = (Run 'refresh') }
        } else {
            @{ svg = (Tile $null 'Minibar' '#8b95a9' 'Press to start'); background = '#10141d'; action = (Run 'start') }
        }
        return Draw $instance @{ button = $button }
    }
    $rows = @(Rows $provider)
    $metric = $rows | Where-Object { $_.id -eq $wanted.metric -or (Metric-Label $provider $_) -eq $wanted.metric } | Select-Object -First 1
    if (-not $metric) { $metric = $rows[0] }
    Draw $instance @{ button = (Quota-Key $provider $metric) }
}

function Draw($instance, $content) {
    $drawing = @{ type = 'draw'; instance = $instance } + $content
    $text = $drawing | ConvertTo-Json -Compress -Depth 12
    if ($sent[$instance] -ne $text) {
        $sent[$instance] = $text
        [Console]::Out.WriteLine($text)
    }
}

function Draw-View($instance) {
    if ($views[$instance].view -eq 'quota') { Draw-Button $instance } else { Draw-Page $instance }
}

function Send-Options {
    if (-not $snapshot) { return } # keep suggesting what was there while Minibar is away
    $providers = @($snapshot.providers | Where-Object { (Rows $_).Count -gt 0 } | ForEach-Object { $_.name })
    $metrics = @($catalog.providers | ForEach-Object { $_.metrics } | ForEach-Object { $_.label } | Select-Object -Unique)
    $text = "$providers|$metrics"
    if ($text -ne $options) {
        $script:options = $text
        Send @{ type = 'options'; key = 'providers'; values = $providers }
        Send @{ type = 'options'; key = 'metrics'; values = $metrics }
    }
}

function Poll {
    $script:snapshot = Ask 'snapshot'
    if ($snapshot -and (-not $catalog -or $catalogAge.Elapsed.TotalSeconds -ge $catalogSeconds)) {
        $script:catalog = Ask 'catalog'
        $catalogAge.Restart()
    }
    Send-Options
    foreach ($instance in @($views.Keys)) { Draw-View $instance }
    $pollAge.Restart()
}

# Console.In blocks even when asked not to, and quotas have to refresh between messages.
$stdin = [IO.StreamReader]::new([Console]::OpenStandardInput(), [Text.UTF8Encoding]::new($false))
$pending = $stdin.ReadLineAsync()
$pollAge = [Diagnostics.Stopwatch]::StartNew()
$catalogAge = [Diagnostics.Stopwatch]::StartNew()
Poll
while ($true) {
    if ($pending.Wait(500)) {
        $line = $pending.Result
        if ($null -eq $line) { break }
        $pending = $stdin.ReadLineAsync()
        $message = $line | ConvertFrom-Json
        switch ($message.type) {
            'appear' {
                $views[$message.instance] = $message
                Draw-View $message.instance
            }
            'disappear' {
                $views.Remove($message.instance)
                $sent.Remove($message.instance)
            }
            'run' {
                switch ($message.action) {
                    'popup' {
                        $request = if ($message.params.provider) { @{ provider = $message.params.provider } } else { @{} }
                        if (-not (Ask 'open_popup' $request)) { Start-Minibar }
                    }
                    'refresh' { Ask 'refresh' | Out-Null }
                    'start' { Start-Minibar }
                    'more' {
                        $instance = $message.params.instance
                        $offsets[$instance] = [int]$offsets[$instance] + 5
                        if ($views[$instance]) { Draw-Page $instance }
                    }
                }
            }
        }
    }
    if ($views.Count -gt 0 -and $pollAge.Elapsed.TotalSeconds -ge $pollSeconds) { Poll }
}
