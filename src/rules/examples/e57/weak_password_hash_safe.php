<?php

function hashPassword(string $password): string
{
    return password_hash($password, PASSWORD_DEFAULT);
}

function checksum(string $data): string
{
    return md5($data);
}

function digest(string $password): string
{
    return hash('sha256', $password);
}
