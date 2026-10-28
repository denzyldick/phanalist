<?php

function stream_real_bad(string $path): array
{
    $handle = fopen($path, 'rb');
    if ($handle === false) {
        return [];
    }

    $rows = [];
    while (($line = fgets($handle)) !== false) {
        $rows[] = $line;
    }

    fclose($handle);

    return $rows;
}
