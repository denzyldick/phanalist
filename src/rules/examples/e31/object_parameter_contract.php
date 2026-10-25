<?php

class ParameterContractHelper
{
    /**
     * @return int<3, 3>
     */
    public function three(): int
    {
        return 3;
    }
}

function reports_zero_from_object_parameter_contract(
    ParameterContractHelper $helper
): int {
    return 100 / ($helper->three() - 3);
}
