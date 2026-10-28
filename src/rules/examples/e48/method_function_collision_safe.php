<?php

final class Payload
{
    public function unserialize(string $value): void
    {
        $decoded = unserialize($value);
    }
}
