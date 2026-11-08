<?php
// Bad: weak cryptographic hash for passwords
$hash = md5($password);
$hash = sha1($password);
