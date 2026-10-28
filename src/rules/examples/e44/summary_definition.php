<?php

function nested_helper(array $items): void
{
    foreach ($items as $left) {
        foreach ($items as $right) {
            echo $left . $right;
        }
    }
}
