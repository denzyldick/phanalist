<?php

function decodedName(string $json): string
{
    return json_decode($json)->name;
}

function firstItem(string $json): mixed
{
    return json_decode($json, true)[0];
}
