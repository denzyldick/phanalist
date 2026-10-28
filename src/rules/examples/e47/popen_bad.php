<?php

function countOutput(string $command): int
{
    $pipe = popen($command, 'r');
    $lines = 0;

    while (!feof($pipe)) {
        if (fgets($pipe) !== false) {
            $lines++;
        }
    }

    return $lines;
}
