<?php

class ObjectContractHelper
{
    /**
     * @return int<3, 3>
     */
    public function three(): int
    {
        return 3;
    }
}

function reports_zero_from_object_method_contract(): int
{
    $helper = new ObjectContractHelper();
    $value = $helper->three();

    return 100 / ($value - 3);
}
