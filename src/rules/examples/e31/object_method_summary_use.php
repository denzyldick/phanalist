<?php

function reports_zero_from_cross_file_object_contract(): int
{
    $helper = new CrossFileObjectContract();
    $value = $helper->three();

    return 100 / ($value - 3);
}
