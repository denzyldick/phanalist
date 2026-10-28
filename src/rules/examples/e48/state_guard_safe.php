<?php

final class RetryingWriter
{
    private bool $retrying = false;

    public function write(string $record): void
    {
        if (!$this->retrying) {
            $this->retrying = true;
            $this->write($record);
        }
    }
}
