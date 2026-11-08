<?php
// Safe: LDAP input is escaped
$result = ldap_search($conn, "ou=users", "(cn=static)");
