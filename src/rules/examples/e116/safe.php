<?php
// Safe: popen with static command
$handle = popen("cat /etc/hostname", "r");
