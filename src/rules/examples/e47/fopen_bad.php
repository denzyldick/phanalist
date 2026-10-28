<?php

function readLog(string $path): string
{
    $handle = fopen($path, 'r');
    $contents = stream_get_contents($handle);

    return $contents;
}
