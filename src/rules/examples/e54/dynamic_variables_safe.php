<?php

function hydrate(array $input): array
{
    $name = $input['name'] ?? '';
    $email = $input['email'] ?? '';

    return compact('name', 'email');
}
