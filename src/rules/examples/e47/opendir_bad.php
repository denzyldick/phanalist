<?php

function directoryNames(string $directory): array
{
    $handle = opendir($directory);
    $names = [];

    while (($entry = readdir($handle)) !== false) {
        $names[] = $entry;
    }

    return $names;
}
