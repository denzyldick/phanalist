<?php

function handOff(string $path): void
{
    $handle = fopen($path, 'r');
    registerHandle($handle);
}

function registerHandle($handle): void
{
    fclose($handle);
}
