<?php

function intdiv_overflow(): int
{
    return intdiv(-9223372036854775808, -1);
}
