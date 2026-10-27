<?php

$stream = fopen('php://memory', 'r');
fclose($stream);
fread($stream, 10);
