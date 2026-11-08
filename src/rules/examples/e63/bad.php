<?php
// Bad: LDAP injection via user-controlled input
$cn = $_GET['cn'] ?? '';
$result = ldap_search($conn, "ou=users", "(cn=" . $cn . ")");
$result = ldap_list($conn, "ou=users", "(uid=" . $_POST['uid'] . ")");
