<?php

function unused(string $path): void
{
    $handle = fopen($path, 'r');
}
