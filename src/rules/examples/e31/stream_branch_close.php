<?php

$stream = fopen('php://memory', 'r');
if ($condition) {
    fclose($stream);
}
fread($stream, 10);
