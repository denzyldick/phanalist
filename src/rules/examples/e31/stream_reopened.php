<?php

$stream = fopen('php://memory', 'r');
fclose($stream);
$stream = fopen('php://memory', 'r');
fread($stream, 10);
fclose($stream);
