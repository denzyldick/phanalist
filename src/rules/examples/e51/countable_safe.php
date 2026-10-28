<?php

function total(array $items, Countable $countable): int
{
    return count($items) + sizeof($countable) + count(['a', 'b']);
}
