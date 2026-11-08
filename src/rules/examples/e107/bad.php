<?php
// Bad: weak random for security token
$token = mt_rand(1000, 9999);
