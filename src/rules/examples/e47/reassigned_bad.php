<?php

function overwrite(string $path): void
{
    $handle = fopen($path, 'r');
    fwrite($handle, 'x');
    $handle = null;
}
