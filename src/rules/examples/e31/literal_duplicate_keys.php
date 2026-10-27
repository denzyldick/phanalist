<?php

function duplicate_key_uses_last_value(): int
{
    $values = [1 => 5, 1 => 0];

    return 10 / $values[1];
}
