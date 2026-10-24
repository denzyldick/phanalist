<?php

function unreachable_after_return(): int
{
    return 1;

    echo "never";
}
