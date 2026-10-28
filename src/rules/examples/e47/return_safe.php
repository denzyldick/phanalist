<?php

function openLog(string $path)
{
    $handle = fopen($path, 'r');

    return $handle;
}
