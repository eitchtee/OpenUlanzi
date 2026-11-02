# A whole plugin: says what to suggest, then handles one JSON line per action until stdin closes.
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

[Console]::Out.WriteLine('{"type":"options","key":"notes","values":["Call back","Buy milk"]}')

while ($null -ne ($line = [Console]::In.ReadLine())) {
    $message = $line | ConvertFrom-Json
    if ($message.type -eq 'run' -and $message.action -eq 'write') {
        Add-Content -Path notes.txt -Value $message.params.text -Encoding UTF8
    }
}
