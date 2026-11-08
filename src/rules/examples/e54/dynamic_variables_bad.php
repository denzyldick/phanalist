<?php

function hydrate(): void
{
    $input = $_POST['input'] ?? [];
    $name = $_POST['name'] ?? 'key';
    $key = $_POST['key'] ?? 'val';

    extract($input);

    $$name = 'value';
    ${$key} = 'value';
}
