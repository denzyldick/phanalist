<?php

function nullsafe_method_call(): void
{
    $value = null;
    $value?->run();
}
