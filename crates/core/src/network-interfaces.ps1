$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()

# Bulk enumeration allows adapters without DNS/IPv4 records to join as empty sets.
$adapters = @(Get-NetAdapter -ErrorAction Stop | Where-Object { $_.Status -eq 'Up' })
$dnsRecords = @(Get-DnsClientServerAddress -ErrorAction Stop | Where-Object { $_.AddressFamily -eq 2 })
$addresses = @(Get-NetIPAddress -ErrorAction Stop | Where-Object {
    $_.AddressFamily -eq 2 -and $_.AddressState -eq 'Preferred' -and
    $_.IPAddress -notlike '169.254.*' -and $_.IPAddress -ne '127.0.0.1'
})
$rows = @(foreach ($adapter in $adapters) {
    $dns = @($dnsRecords | Where-Object { $_.InterfaceIndex -eq $adapter.ifIndex } | ForEach-Object { $_.ServerAddresses })
    foreach ($address in @($addresses | Where-Object { $_.InterfaceIndex -eq $adapter.ifIndex })) {
        [pscustomobject]@{
            id = '{0}/{1}' -f $adapter.InterfaceGuid.ToString(), $address.IPAddress
            name = $adapter.Name
            ipv4 = $address.IPAddress
            prefixLength = $address.PrefixLength
            dnsServers = @($dns)
            eligible = [bool]$adapter.HardwareInterface
        }
    }
})
ConvertTo-Json -InputObject $rows -Depth 4 -Compress
