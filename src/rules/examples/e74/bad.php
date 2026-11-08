<?php
// Bad: weak random number for security-sensitive value
$token = mt_rand(1000, 9999);
$key = rand();
