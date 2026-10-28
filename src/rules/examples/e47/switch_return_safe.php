<?php

function openOrClose(string $path, bool $returnHandle)
{
    $handle = fopen($path, 'r');
    fwrite($handle, 'x');

    switch ($returnHandle) {
        case true:
            return $handle;
        default:
            fclose($handle);
    }
}
