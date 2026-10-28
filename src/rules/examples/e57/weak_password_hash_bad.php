<?php

function hashPassword(string $password): string
{
    return md5($password);
}

function hashPasswordAgain(string $password): string
{
    return hash('sha1', $password);
}

function hashUserPassword(object $user): string
{
    return sha1($user->password);
}
