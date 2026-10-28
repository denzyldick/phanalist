<?php

function nested_bad(array $items): void
{
    foreach ($items as $outer) {
        foreach ($items as $inner) {
            echo $outer . $inner;
        }
    }
}
