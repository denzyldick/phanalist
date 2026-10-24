<?php

function try_finally_after_return(): void
{
    try {
        return;
    } finally {
        $zero = 0;
        echo 1 / $zero;
    }
}
