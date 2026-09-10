## Default Permission

把 CampusLogin 进程网络绑定到 WLAN(ConnectivityManager.bindProcessToNetwork),
强制登录流量走 WiFi 而非移动数据。

#### This default permission set includes the following:

- `allow-bind-to-wifi`
- `allow-unbind`

## Permission Table

<table>
<tr>
<th>Identifier</th>
<th>Description</th>
</tr>


<tr>
<td>

`campus-network-bind:allow-bindToWifi`

</td>
<td>

Enables the bindToWifi command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-network-bind:deny-bindToWifi`

</td>
<td>

Denies the bindToWifi command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-network-bind:allow-unbind`

</td>
<td>

Enables the unbind command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-network-bind:deny-unbind`

</td>
<td>

Denies the unbind command without any pre-configured scope.

</td>
</tr>
</table>
