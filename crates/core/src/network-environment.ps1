param([string]$Ps5Ip, [string]$LanIp, [string[]]$Upstreams)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$checks = @()
$adapters = @(Get-NetAdapter -ErrorAction Stop | Where-Object { $_.Status -eq 'Up' })
$tunnels = @($adapters | Where-Object {
    -not $_.HardwareInterface -and
    ('{0} {1}' -f $_.Name, $_.InterfaceDescription) -match '(?i)tun|tap|vpn|wireguard|clash|mihomo|sing.?box|bettbox'
})
if ($tunnels.Count -gt 0) {
    $checks += [pscustomobject]@{id='tunnel';title='TUN / VPN 网卡';status='warn';detail=('检测到 {0}。接收地址仍使用物理网卡；请在代理中直连 PS5、本机局域网地址和 127.0.0.1，避免 TUN 接管局域网链路。电脑开启 TUN 不会自动代理 PS5。' -f (($tunnels | ForEach-Object Name) -join '、'))}
} else {
    $checks += [pscustomobject]@{id='tunnel';title='TUN / VPN 网卡';status='pass';detail='未发现名称或驱动符合 TUN / VPN 特征的活动虚拟网卡；此检查不代表系统一定没有代理。'}
}
if ($Ps5Ip -and $LanIp) {
    $route = Find-NetRoute -RemoteIPAddress $Ps5Ip -ErrorAction Stop | Where-Object { $_.IPAddress } | Select-Object -First 1
    if ($route -and $route.IPAddress -eq $LanIp) {
        $checks += [pscustomobject]@{id='ps5Route';title='到 PS5 的网络路由';status='pass';detail='Windows 当前选择所选局域网地址连接 PS5。实际防火墙与主机可达性仍需开播验证。'}
    } else {
        $checks += [pscustomobject]@{id='ps5Route';title='到 PS5 的网络路由';status='warn';detail='Windows 到 PS5 的首选路由与所选物理网卡不一致，请检查 TUN 路由、子网重叠及代理的局域网直连规则。'}
    }
}
foreach ($server in $Upstreams) {
    $route = Find-NetRoute -RemoteIPAddress $server -ErrorAction Stop | Where-Object { $_.InterfaceIndex } | Select-Object -First 1
    if ($route -and @($tunnels | Where-Object { $_.ifIndex -eq $route.InterfaceIndex }).Count -gt 0) {
        $checks += [pscustomobject]@{id=('dnsRoute-' + $server);title='上游 DNS 路由';status='warn';detail=('到 {0} 的系统路由经过 TUN / VPN。若解析失败，请检查代理的 DNS 劫持、fake-IP 和直连规则；应用不会自动关闭代理或修改路由。' -f $server)}
    }
}
ConvertTo-Json -InputObject @($checks) -Depth 4 -Compress
