$ErrorActionPreference = 'Stop'

function Get-NetAdapter {
    @([pscustomobject]@{Status='Up';ifIndex=52;InterfaceGuid=[guid]::NewGuid();Name='Virtual without IPv4';HardwareInterface=$false},
      [pscustomobject]@{Status='Up';ifIndex=7;InterfaceGuid=[guid]::NewGuid();Name='Ethernet';HardwareInterface=$true},
      [pscustomobject]@{Status='Up';ifIndex=8;InterfaceGuid=[guid]::NewGuid();Name='Ethernet without DNS';HardwareInterface=$true})
}
function Get-DnsClientServerAddress {
    [pscustomobject]@{InterfaceIndex=7;AddressFamily=2;ServerAddresses=@('192.168.1.1')}
}
function Get-NetIPAddress {
    @([pscustomobject]@{InterfaceIndex=7;AddressFamily=2;AddressState='Preferred';IPAddress='192.168.1.2';PrefixLength=24},
      [pscustomobject]@{InterfaceIndex=7;AddressFamily=2;AddressState='Preferred';IPAddress='192.168.2.2';PrefixLength=24},
      [pscustomobject]@{InterfaceIndex=8;AddressFamily=2;AddressState='Preferred';IPAddress='192.168.3.2';PrefixLength=24})
}
$script = Join-Path $PSScriptRoot '../crates/core/src/network-interfaces.ps1'
$rows = @((& $script) | ConvertFrom-Json | ForEach-Object { $_ })
if ($rows.Count -ne 3) { throw 'Missing IPv4 records must not prevent other adapters from being enumerated.' }
if (@($rows | Where-Object name -eq 'Ethernet').Count -ne 2) { throw 'Both physical adapter IPv4 addresses must remain selectable.' }
if (@($rows.id | Select-Object -Unique).Count -ne 3) { throw 'Each address must have a unique interface identifier.' }
$withoutDns = $rows | Where-Object name -eq 'Ethernet without DNS'
if ($withoutDns.dnsServers -isnot [array] -or $withoutDns.dnsServers.Count -ne 0) { throw 'An adapter without DNS must have an empty array.' }
if ($rows[0].dnsServers -isnot [array] -or $rows[0].dnsServers[0] -ne '192.168.1.1') { throw 'Single DNS server must retain the array wire shape.' }
Write-Output 'Network enumeration regression passed: missing DNS/IPv4, multi-address selection and array shapes.'
