<?php

function issueToken(): string
{
    $token = bin2hex(random_bytes(32));

    return $token;
}

function sample(): int
{
    return rand(1, 10);
}

function digest(string $value): string
{
    return hash('sha256', $value);
}
