<?php

class ParameterContractBase
{
    /**
     * @return int<3, 3>
     */
    public function three(): int
    {
        return 3;
    }
}

class ParameterContractChild extends ParameterContractBase
{
    public function three(): int
    {
        return 4;
    }
}

function keeps_subclass_override_unknown(ParameterContractBase $helper): int
{
    return 100 / ($helper->three() - 3);
}
