<?php

function do_while_return_unreachable(): void
{
    do {
        return;
    } while (true);

    echo "unreachable";
}
