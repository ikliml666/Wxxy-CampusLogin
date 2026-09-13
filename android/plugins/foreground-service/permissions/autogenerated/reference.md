## Default Permission

校园网后台监控保活:前台服务(常驻通知+WifiLock+WakeLock)与开机自启开关。
业务逻辑(检测/自动重登)全在 Rust 侧,本插件仅保证进程存活与通知展示。

#### This default permission set includes the following:

- `allow-startMonitor`
- `allow-stopMonitor`
- `allow-updateNotification`
- `allow-setBootAutostart`
- `allow-isBootAutostartEnabled`
- `allow-installApk`
- `allow-beginProbeWindow`
- `allow-endProbeWindow`
- `allow-getPowerState`
- `allow-getBatteryOptimizationInfo`
- `allow-requestIgnoreBatteryOptimizations`
- `allow-openVendorBatterySettings`

## Permission Table

<table>
<tr>
<th>Identifier</th>
<th>Description</th>
</tr>


<tr>
<td>

`campus-monitor-service:allow-installApk`

</td>
<td>

Enables the installApk command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:deny-installApk`

</td>
<td>

Denies the installApk command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:allow-isBootAutostartEnabled`

</td>
<td>

Enables the isBootAutostartEnabled command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:deny-isBootAutostartEnabled`

</td>
<td>

Denies the isBootAutostartEnabled command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:allow-setBootAutostart`

</td>
<td>

Enables the setBootAutostart command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:deny-setBootAutostart`

</td>
<td>

Denies the setBootAutostart command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:allow-startMonitor`

</td>
<td>

Enables the startMonitor command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:deny-startMonitor`

</td>
<td>

Denies the startMonitor command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:allow-stopMonitor`

</td>
<td>

Enables the stopMonitor command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:deny-stopMonitor`

</td>
<td>

Denies the stopMonitor command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:allow-updateNotification`

</td>
<td>

Enables the updateNotification command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-monitor-service:deny-updateNotification`

</td>
<td>

Denies the updateNotification command without any pre-configured scope.

</td>
</tr>
</table>
