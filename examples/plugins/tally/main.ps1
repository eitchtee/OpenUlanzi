# A whole plugin: a page it draws by itself, and a button that keeps itself up to date.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

$views = @{}   # what is on a deck now, by instance
$scores = @{}  # kept after a view leaves, so a page comes back as it was

function Send($message) {
    [Console]::Out.WriteLine(($message | ConvertTo-Json -Compress -Depth 9))
}

function Text($width, $text, $color) {
    "<svg xmlns='http://www.w3.org/2000/svg' width='$width' height='196'><text x='$($width / 2)' y='125' text-anchor='middle' font-family='Bahnschrift' font-size='72' fill='$color'>$text</text></svg>"
}

# What a press should do comes back to us as an ordinary plugin action.
function Run($action, $instance, $team) {
    @{ plugin = @{ id = 'example.tally'; action = $action; params = @{ instance = $instance; team = $team } } }
}

function Draw-Board($instance) {
    $view = $views[$instance]
    $score = $scores[$instance]
    Send @{ type = 'draw'; instance = $instance; keys = @{
        '0'    = @{ label = $view.params.left; icon = 'ph:plus-bold'; action = (Run 'add' $instance 'left') }
        '4'    = @{ label = $view.params.right; icon = 'ph:plus-bold'; action = (Run 'add' $instance 'right') }
        '2'    = @{ label = 'Hold to clear'; icon = 'ph:arrow-counter-clockwise-bold'; hold = (Run 'reset' $instance '') }
        '12'   = @{ label = 'Back'; icon = 'ph:arrow-left-bold'; action = 'back' }
        'wide' = @{ svg = (Text 458 "$($score.left) x $($score.right)" '#f4f6fb'); background = '#101522' }
    } }
}

function Draw-Clock($instance) {
    $view = $views[$instance]
    Send @{ type = 'draw'; instance = $instance; button = @{
        svg = (Text 196 (Get-Date -Format 'HH:mm') $view.params.color); background = '#101522'
        action = @{ open = 'ms-clock:' }
    } }
}

# Console.In blocks even when asked not to, and the clock has to tick between messages.
$stdin = [IO.StreamReader]::new([Console]::OpenStandardInput(), [Text.UTF8Encoding]::new($false))
$pending = $stdin.ReadLineAsync()
$minute = ''
while ($true) {
    if ($pending.Wait(1000)) {
        $line = $pending.Result
        if ($null -eq $line) { break }
        $pending = $stdin.ReadLineAsync()
        $message = $line | ConvertFrom-Json
        switch ($message.type) {
            'appear' {
                $views[$message.instance] = $message
                if ($message.view -eq 'clock') {
                    Draw-Clock $message.instance
                } else {
                    if (-not $scores[$message.instance]) { $scores[$message.instance] = @{ left = 0; right = 0 } }
                    Draw-Board $message.instance
                }
            }
            'disappear' { $views.Remove($message.instance) }
            'run' {
                $instance = $message.params.instance
                if ($message.action -eq 'add') { $scores[$instance][$message.params.team]++ }
                if ($message.action -eq 'reset') { $scores[$instance] = @{ left = 0; right = 0 } }
                if ($views[$instance]) { Draw-Board $instance }
            }
        }
    }
    if ($minute -ne (Get-Date -Format 'HH:mm')) {
        $minute = Get-Date -Format 'HH:mm'
        foreach ($view in @($views.Values | Where-Object { $_.view -eq 'clock' })) { Draw-Clock $view.instance }
    }
}
