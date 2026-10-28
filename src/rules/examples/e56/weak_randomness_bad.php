<?php

function issueToken(): string
{
    $weakToken = uniqid();

    return password_hash(mt_rand(), PASSWORD_DEFAULT);
}

function issueNonce(): string
{
    $nonce = (string) rand();

    return $nonce;
}
