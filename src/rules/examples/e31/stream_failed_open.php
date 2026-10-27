<?php

$stream = fopen('/missing', 'r');
if ($stream === false) {
    return;
}
fread($stream, 10);
fclose($stream);
