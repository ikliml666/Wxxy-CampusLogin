## Default Permission

CampusLogin 密码落盘加密:AndroidKeyStore AES-GCM 加解密原语,
密钥不出安全硬件,替代桌面端 DPAPI。

#### This default permission set includes the following:

- `allow-encrypt`
- `allow-decrypt`

## Permission Table

<table>
<tr>
<th>Identifier</th>
<th>Description</th>
</tr>


<tr>
<td>

`campus-keystore:allow-decrypt`

</td>
<td>

Enables the decrypt command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-keystore:deny-decrypt`

</td>
<td>

Denies the decrypt command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-keystore:allow-encrypt`

</td>
<td>

Enables the encrypt command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`campus-keystore:deny-encrypt`

</td>
<td>

Denies the encrypt command without any pre-configured scope.

</td>
</tr>
</table>
