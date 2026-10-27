<?php

$stream = fopen('php://memory', 'r');
$alias = $stream;
fclose($stream);
fread($alias, 10);
