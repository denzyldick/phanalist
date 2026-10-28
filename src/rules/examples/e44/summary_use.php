<?php

function summary_use(array $items): void
{
    foreach ($items as $value) {
        nested_helper($items);
    }
}
