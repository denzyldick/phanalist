<?php

function stream_real_safe(string $path): void
{
    $handle = fopen($path, 'rb');
    if ($handle === false) {
        return;
    }

    while (($line = fgets($handle)) !== false) {
        echo $line;
    }

    fclose($handle);
}
