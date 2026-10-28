<?php

function readLog(string $path): string
{
    $handle = fopen($path, 'r');

    if ($handle === false) {
        return '';
    }

    $contents = stream_get_contents($handle);
    fclose($handle);

    return $contents;
}
