<?php

function null_keys_use_php_string_keys(): int
{
    $values = [null => 0, 5];

    return 10 / $values[0];
}
