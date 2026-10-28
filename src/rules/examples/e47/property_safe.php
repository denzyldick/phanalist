<?php

final class LogReader
{
    private $handle;

    public function open(string $path): void
    {
        $this->handle = fopen($path, 'r');
    }
}
