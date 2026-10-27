<?php

$stream = fopen('php://memory', 'w');
fclose($stream);
fwrite($stream, 'data');
