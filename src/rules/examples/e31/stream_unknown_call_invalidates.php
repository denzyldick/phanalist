<?php

$stream = fopen('php://memory', 'r');
consume($stream);
fread($stream, 10);
fclose($stream);
