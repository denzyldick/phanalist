<?php

function hydrate(array $input, string $name, string $key): void
{
    extract($input);

    $$name = 'value';
    ${$key} = 'value';
}
